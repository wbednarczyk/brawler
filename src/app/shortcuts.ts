import type { ShortcutBindingSetting } from "../api/types";
import type { ShortcutDefinition, ShortcutKeyBinding, ShortcutReferenceItem } from "../shared/shortcuts";
import type { Section } from "./navigation";

export type AppShortcutId =
  | "app.openInbox"
  | "app.openCompanies"
  | "app.openWatchlists"
  | "app.openNotebooks"
  | "app.openEvents"
  | "app.openTranscripts"
  | "app.openSources"
  | "app.openSettings"
  | "app.commandPalette"
  | "app.focusSearch"
  | "app.refreshSources"
  | "app.refreshDatabase"
  | "inbox.nextItem"
  | "inbox.previousItem"
  | "inbox.toggleRead"
  | "inbox.toggleSaved"
  | "inbox.openSource"
  | "inbox.createNote"
  | "company.nextCompany"
  | "company.previousCompany"
  | "company.nextTab"
  | "company.previousTab"
  | "notebook.editSelected"
  | "notebook.saveCurrent";

export type AppShortcutActionMap = Record<AppShortcutId, () => boolean | void>;

export type AppShortcutReferenceItem = ShortcutReferenceItem & {
  id: AppShortcutId;
  defaultBinding: ShortcutKeyBinding;
  disabled: boolean;
  hasCustomBinding: boolean;
};

const navigationShortcuts = [
  ["app.openInbox", "Open Inbox", "1", "Inbox"],
  ["app.openCompanies", "Open Companies", "2", "Companies"],
  ["app.openWatchlists", "Open Watchlists", "3", "Watchlists"],
  ["app.openNotebooks", "Open Notebooks", "4", "Notebooks"],
  ["app.openEvents", "Open Events", "5", "Events"],
  ["app.openTranscripts", "Open Transcripts", "6", "Transcripts"],
  ["app.openSources", "Open Sources", "7", "Sources"],
  ["app.openSettings", "Open Settings", "8", "Settings"],
] satisfies Array<[AppShortcutId, string, string, Section]>;

export const appShortcutReferenceItems: AppShortcutReferenceItem[] = [
  ...navigationShortcuts.map(([id, label, key]) => ({
    id,
    label,
    group: "Navigation",
    scope: "app" as const,
    defaultBinding: {
      ctrlKey: true,
      key,
    },
    binding: {
      ctrlKey: true,
      key,
    },
    disabled: false,
    hasCustomBinding: false,
  })),
  {
    id: "app.commandPalette",
    label: "Open command palette",
    group: "Global actions",
    scope: "app",
    binding: {
      ctrlKey: true,
      key: "K",
    },
    defaultBinding: {
      ctrlKey: true,
      key: "K",
    },
    disabled: false,
    hasCustomBinding: false,
  },
  {
    id: "app.focusSearch",
    label: "Focus Inbox search",
    group: "Global actions",
    scope: "app",
    binding: {
      ctrlKey: true,
      key: "F",
    },
    defaultBinding: {
      ctrlKey: true,
      key: "F",
    },
    disabled: false,
    hasCustomBinding: false,
  },
  {
    id: "app.refreshSources",
    label: "Refresh sources",
    group: "Global actions",
    scope: "app",
    binding: {
      key: "F9",
    },
    defaultBinding: {
      key: "F9",
    },
    disabled: false,
    hasCustomBinding: false,
  },
  {
    id: "app.refreshDatabase",
    label: "Refresh workspace data",
    group: "Global actions",
    scope: "app",
    binding: {
      shiftKey: true,
      key: "F9",
    },
    defaultBinding: {
      shiftKey: true,
      key: "F9",
    },
    disabled: false,
    hasCustomBinding: false,
  },
  {
    id: "inbox.nextItem",
    label: "Select next inbox item",
    group: "Inbox",
    scope: "screen",
    binding: {
      key: "J",
    },
    defaultBinding: {
      key: "J",
    },
    disabled: false,
    hasCustomBinding: false,
  },
  {
    id: "inbox.previousItem",
    label: "Select previous inbox item",
    group: "Inbox",
    scope: "screen",
    binding: {
      key: "K",
    },
    defaultBinding: {
      key: "K",
    },
    disabled: false,
    hasCustomBinding: false,
  },
  {
    id: "inbox.toggleRead",
    label: "Toggle inbox read state",
    group: "Inbox",
    scope: "screen",
    binding: {
      key: "M",
    },
    defaultBinding: {
      key: "M",
    },
    disabled: false,
    hasCustomBinding: false,
  },
  {
    id: "inbox.toggleSaved",
    label: "Toggle inbox saved state",
    group: "Inbox",
    scope: "screen",
    binding: {
      key: "S",
    },
    defaultBinding: {
      key: "S",
    },
    disabled: false,
    hasCustomBinding: false,
  },
  {
    id: "inbox.openSource",
    label: "Open selected inbox source",
    group: "Inbox",
    scope: "screen",
    binding: {
      key: "O",
    },
    defaultBinding: {
      key: "O",
    },
    disabled: false,
    hasCustomBinding: false,
  },
  {
    id: "inbox.createNote",
    label: "Create note from selected inbox item",
    group: "Inbox",
    scope: "screen",
    binding: {
      key: "N",
    },
    defaultBinding: {
      key: "N",
    },
    disabled: false,
    hasCustomBinding: false,
  },
  {
    id: "company.nextCompany",
    label: "Select next company",
    group: "Companies",
    scope: "screen",
    binding: {
      shiftKey: true,
      key: "J",
    },
    defaultBinding: {
      shiftKey: true,
      key: "J",
    },
    disabled: false,
    hasCustomBinding: false,
  },
  {
    id: "company.previousCompany",
    label: "Select previous company",
    group: "Companies",
    scope: "screen",
    binding: {
      shiftKey: true,
      key: "K",
    },
    defaultBinding: {
      shiftKey: true,
      key: "K",
    },
    disabled: false,
    hasCustomBinding: false,
  },
  {
    id: "company.nextTab",
    label: "Open next company tab",
    group: "Companies",
    scope: "screen",
    binding: {
      key: "L",
    },
    defaultBinding: {
      key: "L",
    },
    disabled: false,
    hasCustomBinding: false,
  },
  {
    id: "company.previousTab",
    label: "Open previous company tab",
    group: "Companies",
    scope: "screen",
    binding: {
      key: "H",
    },
    defaultBinding: {
      key: "H",
    },
    disabled: false,
    hasCustomBinding: false,
  },
  {
    id: "notebook.editSelected",
    label: "Edit selected notebook entry",
    group: "Notebooks",
    scope: "screen",
    binding: {
      ctrlKey: true,
      key: "E",
    },
    defaultBinding: {
      ctrlKey: true,
      key: "E",
    },
    disabled: false,
    hasCustomBinding: false,
  },
  {
    id: "notebook.saveCurrent",
    label: "Save current notebook edit",
    group: "Notebooks",
    scope: "screen",
    binding: {
      ctrlKey: true,
      key: "S",
    },
    defaultBinding: {
      ctrlKey: true,
      key: "S",
    },
    suppressWhenEditable: false,
    disabled: false,
    hasCustomBinding: false,
  },
];

export function resolveAppShortcutReferenceItems(
  shortcutBindings: Record<string, ShortcutBindingSetting>,
): AppShortcutReferenceItem[] {
  return appShortcutReferenceItems.map((item) => {
    const override = shortcutBindings[item.id];
    const binding = override
      ? {
          key: override.key,
          altKey: override.altKey,
          ctrlKey: override.ctrlKey,
          metaKey: override.metaKey,
          shiftKey: override.shiftKey,
        }
      : item.defaultBinding;

    return {
      ...item,
      binding,
      disabled: Boolean(override?.disabled),
      hasCustomBinding: Boolean(override),
    };
  });
}

export function createAppShortcutDefinitions(
  actions: AppShortcutActionMap,
  shortcutBindings: Record<string, ShortcutBindingSetting>,
): ShortcutDefinition[] {
  return resolveAppShortcutReferenceItems(shortcutBindings)
    .filter((item) => !item.disabled)
    .map((item) => ({
      ...item,
      action: actions[item.id],
    }));
}

export function appShortcutSectionForId(id: AppShortcutId): Section | null {
  const match = navigationShortcuts.find(([shortcutId]) => shortcutId === id);
  return match?.[3] ?? null;
}
