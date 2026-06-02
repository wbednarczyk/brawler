import { useCallback } from "react";

type KeyboardListNavigationOptions<TItem> = {
  items: TItem[];
  onSelect: (item: TItem) => void;
  selectedItem: TItem | null;
};

export function useKeyboardListNavigation<TItem>({
  items,
  onSelect,
  selectedItem,
}: KeyboardListNavigationOptions<TItem>) {
  return useCallback(
    (event: React.KeyboardEvent<HTMLElement>) => {
      if (items.length === 0 || (event.key !== "ArrowDown" && event.key !== "ArrowUp")) {
        return;
      }

      event.preventDefault();

      const currentIndex = selectedItem ? items.indexOf(selectedItem) : -1;
      const nextIndex =
        event.key === "ArrowDown"
          ? Math.min(currentIndex + 1, items.length - 1)
          : Math.max(currentIndex - 1, 0);

      onSelect(items[nextIndex]);
    },
    [items, onSelect, selectedItem],
  );
}
