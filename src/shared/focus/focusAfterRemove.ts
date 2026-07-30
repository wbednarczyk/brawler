import { useLayoutEffect, useRef } from "react";
import type { RefObject } from "react";

export type FocusAfterRemoveOptions = {
  /** Selector matching each focusable/anchor row inside the list container. */
  rowSelector: string;
  /**
   * Optional selector for the element to focus within the chosen row. Omit when
   * the row element itself is focusable (e.g. ExpandableRow's `role="button"`
   * article); provide it when focus belongs on a control inside a non-focusable
   * row (e.g. a research question's primary select button).
   */
  focusSelector?: string;
};

/**
 * Move keyboard focus to the row that now occupies `removedIndex` after a row
 * has been destroyed, so a destructive action never dumps focus back onto
 * `<body>` (ADR 0076 D9 — focus after a destructive action moves to the next
 * sibling row). When the removed row was the tail, focus clamps to the new last
 * row; when the list is now empty, focus falls back to the list container
 * itself (made programmatically focusable).
 */
export function focusRowAfterRemove(
  container: HTMLElement | null,
  removedIndex: number,
  options: FocusAfterRemoveOptions,
): void {
  if (!container) {
    return;
  }
  const rows = Array.from(container.querySelectorAll<HTMLElement>(options.rowSelector));
  if (rows.length === 0) {
    // Nothing left to land on: keep focus inside the region rather than on body.
    if (container.tabIndex < 0) {
      container.tabIndex = -1;
    }
    container.focus();
    return;
  }
  const targetRow = rows[Math.min(Math.max(removedIndex, 0), rows.length - 1)];
  const target = options.focusSelector
    ? targetRow.querySelector<HTMLElement>(options.focusSelector) ?? targetRow
    : targetRow;
  target.focus();
}

export type UseFocusAfterRemoveResult<T extends HTMLElement> = {
  /** Attach to the list container element that holds the rows. */
  listRef: RefObject<T | null>;
};

/**
 * React binding for {@link focusRowAfterRemove}. Pass the stable keys of the
 * currently rendered rows (in render order); when a key disappears the hook
 * detects the removed index from the previous keys and moves focus to the row
 * that slid into that slot after the DOM commits.
 *
 * It auto-detects removals rather than requiring the delete call site to report
 * an index, so it works when the delete control lives outside the list (e.g. a
 * detail-pane button) and composes with async deletes (confirm → api → refetch)
 * — the list simply shrinks a render later.
 *
 * Focus only moves when the departing row itself had focus (activeElement fell
 * back to `<body>`) or focus was already inside the list. A background refresh
 * that drops a row while the user is working elsewhere never steals focus.
 */
export function useFocusAfterRemove<T extends HTMLElement = HTMLElement>(
  itemKeys: ReadonlyArray<string | number>,
  options: FocusAfterRemoveOptions,
): UseFocusAfterRemoveResult<T> {
  const listRef = useRef<T>(null);
  const prevKeysRef = useRef<ReadonlyArray<string | number>>(itemKeys);
  const { rowSelector, focusSelector } = options;

  useLayoutEffect(() => {
    const prevKeys = prevKeysRef.current;
    prevKeysRef.current = itemKeys;

    if (itemKeys.length >= prevKeys.length) {
      return;
    }
    const nextKeys = new Set(itemKeys);
    const removedIndex = prevKeys.findIndex((key) => !nextKeys.has(key));
    if (removedIndex < 0) {
      return;
    }

    const container = listRef.current;
    if (!container) {
      return;
    }
    // Guard against focus theft: only reclaim focus when the removed row owned
    // it (activeElement collapsed to <body>) or focus was already in the list.
    const active = document.activeElement;
    const focusWasHere = active == null || active === document.body || container.contains(active);
    if (!focusWasHere) {
      return;
    }
    focusRowAfterRemove(container, removedIndex, { rowSelector, focusSelector });
  }, [itemKeys, rowSelector, focusSelector]);

  return { listRef };
}
