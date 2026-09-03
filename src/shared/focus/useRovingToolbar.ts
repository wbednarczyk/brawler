import { useCallback, useEffect, useRef, useState, type KeyboardEvent } from "react";

export type RovingToolbarItemProps = {
  tabIndex: 0 | -1;
  ref: (node: HTMLElement | null) => void;
  onFocus: () => void;
  onKeyDown: (event: KeyboardEvent<HTMLElement>) => void;
};

export type UseRovingToolbarResult = {
  focusedIndex: number;
  itemProps: (index: number) => RovingToolbarItemProps;
  focusItem: (index: number) => void;
};

// APG toolbar roving-tabindex (F3c S1, plan § Design 1): exactly one entry
// carries `tabIndex=0` — the "tab stop" — and it follows the LAST FOCUSED
// entry (not the pressed/active one), per the APG pattern. ArrowRight/Left
// move it with wraparound, Home/End jump to the ends; Enter/Space are the
// item's own (a `Button`) activation, not this hook's concern.
// `selectedIndex` (optional): while the toolbar holds no focus, the tab stop
// follows the selection — a tool opened from the palette or H/L makes
// Tab-from-the-tool land on the pressed entry, never on a stale one. While an
// entry IS focused, arrows move from that entry and the selection never yanks
// the stop away (APG: last-focused wins inside the bar).
export function useRovingToolbar({
  count,
  initialIndex,
  selectedIndex,
}: {
  count: number;
  initialIndex: number;
  selectedIndex?: number;
}): UseRovingToolbarResult {
  const [focusedIndex, setFocusedIndex] = useState(initialIndex);
  const itemRefs = useRef<Array<HTMLElement | null>>([]);

  useEffect(() => {
    if (selectedIndex === undefined || selectedIndex < 0 || selectedIndex >= count) return;
    const inside = itemRefs.current.some((node) => node !== null && node === document.activeElement);
    if (!inside) setFocusedIndex(selectedIndex);
  }, [selectedIndex, count]);

  // A shrinking `count` must never leave zero tab stops (sol diff R1).
  useEffect(() => {
    setFocusedIndex((current) => (count > 0 && current >= count ? count - 1 : current));
  }, [count]);

  const focusItem = useCallback(
    (index: number) => {
      if (count <= 0) return;
      const wrapped = ((index % count) + count) % count;
      setFocusedIndex(wrapped);
      itemRefs.current[wrapped]?.focus();
    },
    [count],
  );

  const itemProps = useCallback(
    (index: number): RovingToolbarItemProps => ({
      tabIndex: index === focusedIndex ? 0 : -1,
      ref: (node) => {
        itemRefs.current[index] = node;
      },
      onFocus: () => setFocusedIndex(index),
      onKeyDown: (event) => {
        switch (event.key) {
          case "ArrowRight":
            event.preventDefault();
            focusItem(index + 1);
            break;
          case "ArrowLeft":
            event.preventDefault();
            focusItem(index - 1);
            break;
          case "Home":
            event.preventDefault();
            focusItem(0);
            break;
          case "End":
            event.preventDefault();
            focusItem(count - 1);
            break;
          default:
            break;
        }
      },
    }),
    [count, focusItem, focusedIndex],
  );

  return { focusedIndex, itemProps, focusItem };
}
