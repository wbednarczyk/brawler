import { useLayoutEffect } from "react";

export type ShortcutScope = "app" | "screen";

export type ShortcutKeyBinding = {
  key: string;
  altKey?: boolean;
  ctrlKey?: boolean;
  metaKey?: boolean;
  shiftKey?: boolean;
};

export type ShortcutDefinition = {
  id: string;
  label: string;
  scope: ShortcutScope;
  binding: ShortcutKeyBinding;
  action: () => boolean | void;
  preventDefault?: boolean;
  stopPropagation?: boolean;
  suppressWhenEditable?: boolean;
};

export type ShortcutReferenceItem = Omit<ShortcutDefinition, "action"> & {
  group: string;
};

function normalizeKey(key: string) {
  return key.length === 1 ? key.toLowerCase() : key;
}

export function shortcutMatchesEvent(binding: ShortcutKeyBinding, event: KeyboardEvent) {
  const normalizedBindingKey = normalizeKey(binding.key);
  const normalizedEventKey = normalizeKey(event.key);
  const normalizedEventCode = event.code.startsWith("Key") || event.code.startsWith("Digit")
    ? normalizeKey(event.code.replace(/^(Key|Digit)/, ""))
    : normalizeKey(event.code);

  return (normalizedBindingKey === normalizedEventKey || normalizedBindingKey === normalizedEventCode)
    && Boolean(binding.altKey) === event.altKey
    && Boolean(binding.ctrlKey) === event.ctrlKey
    && Boolean(binding.metaKey) === event.metaKey
    && Boolean(binding.shiftKey) === event.shiftKey;
}

export function isEditableShortcutTarget(target: EventTarget | null) {
  if (!(target instanceof HTMLElement)) {
    return false;
  }

  if (target.isContentEditable) {
    return true;
  }

  const editableElement = target.closest("input, textarea, select, [contenteditable='true']");
  if (!(editableElement instanceof HTMLElement)) {
    return false;
  }

  if (editableElement instanceof HTMLInputElement) {
    return editableElement.type !== "button"
      && editableElement.type !== "submit"
      && editableElement.type !== "reset";
  }

  return true;
}

export function formatShortcutBinding(binding: ShortcutKeyBinding) {
  const parts = [];

  if (binding.ctrlKey) {
    parts.push("Ctrl");
  }
  if (binding.altKey) {
    parts.push("Alt");
  }
  if (binding.shiftKey) {
    parts.push("Shift");
  }
  if (binding.metaKey) {
    parts.push("Meta");
  }

  parts.push(binding.key.length === 1 ? binding.key.toUpperCase() : binding.key);

  return parts.join("+");
}

export function useKeyboardShortcuts(shortcuts: ShortcutDefinition[]) {
  // Layout effect, not a passive one: the document listener must swap in the
  // same commit that produced the new closures. A passive effect leaves a
  // window between paint and effect flush in which a keydown still hits the
  // PREVIOUS render's handler (stale selection/items) — React 19's scheduling
  // made that window real (caught by the inbox shortcut workflow test).
  useLayoutEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      for (const shortcut of shortcuts) {
        if (!shortcutMatchesEvent(shortcut.binding, event)) {
          continue;
        }

        if (shortcut.suppressWhenEditable !== false && isEditableShortcutTarget(event.target)) {
          return;
        }

        const handled = shortcut.action();
        if (handled === false) {
          return;
        }

        if (shortcut.preventDefault !== false) {
          event.preventDefault();
        }
        if (shortcut.stopPropagation) {
          event.stopPropagation();
        }
        return;
      }
    }

    document.addEventListener("keydown", handleKeyDown, { capture: true });

    return () => {
      document.removeEventListener("keydown", handleKeyDown, { capture: true });
    };
  }, [shortcuts]);
}
