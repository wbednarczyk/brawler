import { describe, expect, it, vi } from "vitest";
import { act, renderHook } from "@testing-library/react";

import { useComboboxListbox, type ComboboxEscapeAction } from "./useComboboxListbox";

type Option = { id: string; label: string };

const OPTIONS: Option[] = [
  { id: "a", label: "Alpha" },
  { id: "b", label: "Beta" },
  { id: "c", label: "Gamma" },
];

function filterByLabel(option: Option, query: string) {
  return option.label.toLowerCase().includes(query.toLowerCase());
}

function fakeKeyEvent(key: string) {
  return { key, preventDefault: vi.fn(), stopPropagation: vi.fn() } as unknown as React.KeyboardEvent<HTMLInputElement>;
}

function setup(overrides: Partial<{ options: Option[]; escapePolicy: (s: { query: string; isOpen: boolean }) => ComboboxEscapeAction; onSelect: (o: Option) => void; onCloseHost: () => void }> = {}) {
  const onSelect = overrides.onSelect ?? vi.fn();
  const onCloseHost = overrides.onCloseHost ?? vi.fn();
  const escapePolicy = overrides.escapePolicy ?? (() => "bubble" as const);
  const hook = renderHook(
    (props: { options: Option[] }) =>
      useComboboxListbox({
        options: props.options,
        getId: (o) => o.id,
        filter: filterByLabel,
        onSelect,
        escapePolicy,
        onCloseHost,
      }),
    { initialProps: { options: overrides.options ?? OPTIONS } },
  );
  return { hook, onSelect, onCloseHost };
}

describe("useComboboxListbox", () => {
  it("Enter, Home and End are inert while the list is closed (no silent selection)", () => {
    const onSelect = vi.fn();
    const { hook } = setup({ onSelect });
    const enter = fakeKeyEvent("Enter");
    act(() => hook.result.current.inputProps.onKeyDown(enter));
    expect(onSelect).not.toHaveBeenCalled();
    expect(enter.preventDefault).not.toHaveBeenCalled();
    act(() => hook.result.current.inputProps.onKeyDown(fakeKeyEvent("End")));
    expect(hook.result.current.activeOption?.id).toBe("a");
    act(() => hook.result.current.open());
    act(() => hook.result.current.inputProps.onKeyDown(fakeKeyEvent("Enter")));
    expect(onSelect).toHaveBeenCalledTimes(1);
  });

  it("activedescendant tracks the first option once open, using its stable id", () => {
    const { hook } = setup();
    expect(hook.result.current.inputProps["aria-activedescendant"]).toBeUndefined();
    act(() => hook.result.current.open());
    expect(hook.result.current.inputProps["aria-activedescendant"]).toContain("-option-a");
  });

  it("ArrowDown/ArrowUp move the active option and wrap-clamp at the ends", () => {
    const { hook } = setup();
    act(() => hook.result.current.open());
    act(() => hook.result.current.inputProps.onKeyDown(fakeKeyEvent("ArrowDown")));
    expect(hook.result.current.activeOption?.id).toBe("b");
    act(() => hook.result.current.inputProps.onKeyDown(fakeKeyEvent("ArrowDown")));
    expect(hook.result.current.activeOption?.id).toBe("c");
    // Clamped, not wrapped, at the last entry.
    act(() => hook.result.current.inputProps.onKeyDown(fakeKeyEvent("ArrowDown")));
    expect(hook.result.current.activeOption?.id).toBe("c");
    act(() => hook.result.current.inputProps.onKeyDown(fakeKeyEvent("ArrowUp")));
    expect(hook.result.current.activeOption?.id).toBe("b");
  });

  it("Home/End jump to the first/last filtered option", () => {
    const { hook } = setup();
    act(() => hook.result.current.open());
    act(() => hook.result.current.inputProps.onKeyDown(fakeKeyEvent("End")));
    expect(hook.result.current.activeOption?.id).toBe("c");
    act(() => hook.result.current.inputProps.onKeyDown(fakeKeyEvent("Home")));
    expect(hook.result.current.activeOption?.id).toBe("a");
  });

  it("filtering narrows the list and resets/clamps the active index to the new first match", () => {
    const { hook } = setup();
    act(() => hook.result.current.open());
    act(() => hook.result.current.inputProps.onKeyDown(fakeKeyEvent("ArrowDown"))); // active -> b
    expect(hook.result.current.activeOption?.id).toBe("b");
    act(() => hook.result.current.setQuery("ga"));
    expect(hook.result.current.filtered.map((o) => o.id)).toEqual(["c"]);
    expect(hook.result.current.activeOption?.id).toBe("c");
  });

  it("stable option ids: the SAME option keeps the SAME id across a filter that still includes it", () => {
    const { hook } = setup();
    act(() => hook.result.current.open());
    const idBefore = hook.result.current.optionProps(OPTIONS[0]).id;
    act(() => hook.result.current.setQuery("al"));
    const idAfter = hook.result.current.optionProps(OPTIONS[0]).id;
    expect(idAfter).toBe(idBefore);
  });

  it("dynamic option removal resets a vanished active id to the first remaining option", () => {
    const { hook, } = setup();
    act(() => hook.result.current.open());
    act(() => hook.result.current.inputProps.onKeyDown(fakeKeyEvent("End"))); // active -> c (Gamma)
    expect(hook.result.current.activeOption?.id).toBe("c");

    hook.rerender({ options: OPTIONS.slice(0, 2) }); // Gamma removed
    expect(hook.result.current.activeOption?.id).toBe("a");
  });

  it("pointer hover sets the active option", () => {
    const { hook } = setup();
    act(() => hook.result.current.open());
    act(() => hook.result.current.optionProps(OPTIONS[2]).onMouseEnter());
    expect(hook.result.current.activeOption?.id).toBe("c");
  });

  it("Enter selects the active option and closes the list", () => {
    const { hook, onSelect } = setup();
    act(() => hook.result.current.open());
    act(() => hook.result.current.inputProps.onKeyDown(fakeKeyEvent("ArrowDown")));
    act(() => hook.result.current.inputProps.onKeyDown(fakeKeyEvent("Enter")));
    expect(onSelect).toHaveBeenCalledWith(OPTIONS[1]);
    expect(hook.result.current.isOpen).toBe(false);
  });

  describe("host Escape policy", () => {
    it("close-list: consumes the event and closes the list without touching the query", () => {
      const { hook } = setup({ escapePolicy: () => "close-list" });
      act(() => hook.result.current.setQuery("al"));
      const event = fakeKeyEvent("Escape");
      act(() => hook.result.current.inputProps.onKeyDown(event));
      expect(event.preventDefault).toHaveBeenCalled();
      expect(event.stopPropagation).toHaveBeenCalled();
      expect(hook.result.current.isOpen).toBe(false);
      expect(hook.result.current.query).toBe("al");
    });

    it("clear: consumes the event and empties the query", () => {
      const { hook } = setup({ escapePolicy: () => "clear" });
      act(() => hook.result.current.setQuery("al"));
      const event = fakeKeyEvent("Escape");
      act(() => hook.result.current.inputProps.onKeyDown(event));
      expect(event.preventDefault).toHaveBeenCalled();
      expect(hook.result.current.query).toBe("");
    });

    it("bubble: touches neither the event nor any state", () => {
      const { hook } = setup({ escapePolicy: () => "bubble" });
      act(() => hook.result.current.open());
      const event = fakeKeyEvent("Escape");
      act(() => hook.result.current.inputProps.onKeyDown(event));
      expect(event.preventDefault).not.toHaveBeenCalled();
      expect(event.stopPropagation).not.toHaveBeenCalled();
      expect(hook.result.current.isOpen).toBe(true);
    });

    it("close-host: consumes the event and calls onCloseHost", () => {
      const { hook, onCloseHost } = setup({ escapePolicy: () => "close-host" });
      const event = fakeKeyEvent("Escape");
      act(() => hook.result.current.inputProps.onKeyDown(event));
      expect(event.preventDefault).toHaveBeenCalled();
      expect(event.stopPropagation).toHaveBeenCalled();
      expect(onCloseHost).toHaveBeenCalledTimes(1);
    });
  });
});
