import { useEffect, useId, useMemo, useState, type KeyboardEvent } from "react";

// Headless APG combobox + listbox controller shared by every typed-filter
// picker (`ComboboxField`, the command palette). Escape is host-specific: the
// host's `escapePolicy` names the action; anything but "bubble" consumes the
// event (`preventDefault` + `stopPropagation`) so no ancestor handles it too.

export type ComboboxEscapeAction = "close-list" | "clear" | "bubble" | "close-host";

export type ComboboxEscapeState = { query: string; isOpen: boolean };

export type UseComboboxListboxOptions<T> = {
  // A list, or a function of the current query for hosts whose options come
  // from an async request keyed by that query (no render where the previous
  // query's rows are still the options).
  options: readonly T[] | ((query: string) => readonly T[]);
  /** Stable per-option identity — option ids stay tied to the OPTION, not its
   * index, so they survive filtering/reordering (the active option keeps
   * pointing at the same item, never silently rebinding to "slot 3"). */
  getId: (option: T) => string;
  filter: (option: T, query: string) => boolean;
  onSelect: (option: T) => void;
  escapePolicy: (state: ComboboxEscapeState) => ComboboxEscapeAction;
  /** Invoked when `escapePolicy` resolves "close-host" — the palette's
   * one-Escape-closes-the-modal contract. The picker's policy never returns
   * "close-host", so it never needs this. */
  onCloseHost?: () => void;
};

export function useComboboxListbox<T>({
  options,
  getId,
  filter,
  onSelect,
  escapePolicy,
  onCloseHost,
}: UseComboboxListboxOptions<T>) {
  const [query, setQueryState] = useState("");
  const [isOpen, setIsOpen] = useState(false);
  const [activeId, setActiveId] = useState<string | null>(null);
  const listId = useId();

  const filtered = useMemo(() => {
    const trimmed = query.trim();
    const resolved = typeof options === "function" ? options(query) : options;
    return trimmed ? resolved.filter((option) => filter(option, trimmed)) : resolved;
  }, [options, query, filter]);

  const activeIndex = activeId === null ? -1 : filtered.findIndex((option) => getId(option) === activeId);

  // The active option fell out of the filtered set → land on the first entry.
  useEffect(() => {
    if (filtered.length === 0) {
      if (activeId !== null) setActiveId(null);
      return;
    }
    if (activeIndex === -1) setActiveId(getId(filtered[0]));
    // eslint-disable-next-line react-hooks/exhaustive-deps -- keyed on the filtered identity + activeIndex only; getId/activeId read at call time
  }, [filtered, activeIndex]);

  const clampedIndex = activeIndex === -1 ? 0 : activeIndex;
  const activeOption: T | undefined = filtered[clampedIndex];

  function optionId(option: T): string {
    return `${listId}-option-${getId(option)}`;
  }

  function setQuery(value: string) {
    setQueryState(value);
    setIsOpen(true);
  }

  function moveActive(delta: number) {
    if (filtered.length === 0) return;
    const next = Math.min(Math.max(clampedIndex + delta, 0), filtered.length - 1);
    setActiveId(getId(filtered[next]));
  }

  function select(option: T | undefined) {
    if (!option) return;
    setIsOpen(false);
    onSelect(option);
  }

  function handleKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setIsOpen(true);
      moveActive(1);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setIsOpen(true);
      moveActive(-1);
    } else if (event.key === "Home" || event.key === "End" || event.key === "Enter") {
      // Only a visible list can be navigated or committed: a closed field
      // must never select the (first) active option on Enter.
      if (!isOpen) return;
      event.preventDefault();
      if (event.key === "Home") {
        if (filtered.length > 0) setActiveId(getId(filtered[0]));
      } else if (event.key === "End") {
        if (filtered.length > 0) setActiveId(getId(filtered[filtered.length - 1]));
      } else {
        select(activeOption);
      }
    } else if (event.key === "Escape") {
      const action = escapePolicy({ query, isOpen });
      if (action === "bubble") return;
      event.preventDefault();
      event.stopPropagation();
      if (action === "close-list") {
        setIsOpen(false);
      } else if (action === "clear") {
        setQueryState("");
      } else if (action === "close-host") {
        onCloseHost?.();
      }
    }
  }

  const inputProps = {
    role: "combobox" as const,
    "aria-expanded": isOpen,
    "aria-autocomplete": "list" as const,
    "aria-controls": listId,
    "aria-activedescendant": isOpen && activeOption ? optionId(activeOption) : undefined,
    value: query,
    onKeyDown: handleKeyDown,
  };

  const listboxProps = {
    role: "listbox" as const,
    id: listId,
  };

  function optionProps(option: T) {
    return {
      id: optionId(option),
      role: "option" as const,
      "aria-selected": getId(option) === activeId,
      onMouseEnter: () => setActiveId(getId(option)),
      onClick: () => select(option),
    };
  }

  return {
    query,
    setQuery,
    isOpen,
    open: () => setIsOpen(true),
    close: () => setIsOpen(false),
    /** Sets the query directly and closes the list, WITHOUT `setQuery`'s
     * open-on-type side effect — for a host that needs to clear/seed the
     * query from outside a keystroke (e.g. `ComboboxField`'s blur reset). */
    reset: (value = "") => {
      setQueryState(value);
      setIsOpen(false);
      setActiveId(null);
    },
    filtered,
    activeOption,
    select,
    inputProps,
    listboxProps,
    optionProps,
  };
}
