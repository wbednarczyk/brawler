import { useEffect, useId, useMemo, useState, type KeyboardEvent } from "react";

// Headless APG combobox+listbox controller (dogfooding wave 2026-09, #3):
// query/open/active-option state, filtering, and keyboard navigation shared
// by every typed-filter picker in the app. TWO consumers land in this wave —
// the Spółka header company picker (`ComboboxField`) and the ⌘K palette
// (`CommandPalette.tsx`, which keeps its own markup but drives it off this
// same hook) — so behavior lives here once instead of drifting per call site.
//
// Escape is host-specific (a picker vs. a modal-hosted palette resolve it
// differently), so the HOST supplies `escapePolicy`: given the current
// {query, isOpen}, it names the action to take. When the policy resolves to
// anything but "bubble" the controller consumes the event
// (`preventDefault()` + `stopPropagation()`, the `SearchField.tsx` idiom) so
// it never also reaches an ancestor's own Escape handler (the Spółka tool
// frame, `Modal`'s own listener). "bubble" touches the event at all — the
// host (or nothing) decides what Escape means from there.

export type ComboboxEscapeAction = "close-list" | "clear" | "bubble" | "close-host";

export type ComboboxEscapeState = { query: string; isOpen: boolean };

export type UseComboboxListboxOptions<T> = {
  options: readonly T[];
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
    return trimmed ? options.filter((option) => filter(option, trimmed)) : options;
  }, [options, query, filter]);

  const activeIndex = activeId === null ? -1 : filtered.findIndex((option) => getId(option) === activeId);

  // Active-option reset/clamp (plan § S2 item 1): the previously active
  // option fell out of the filtered/live set (a keystroke narrowed the list,
  // or the host removed/reordered its options) — land on the first entry
  // rather than keep pointing at a vanished id.
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
    } else if (event.key === "Home") {
      event.preventDefault();
      if (filtered.length > 0) setActiveId(getId(filtered[0]));
    } else if (event.key === "End") {
      event.preventDefault();
      if (filtered.length > 0) setActiveId(getId(filtered[filtered.length - 1]));
    } else if (event.key === "Enter") {
      event.preventDefault();
      select(activeOption);
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
    onFocus: () => setIsOpen(true),
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
