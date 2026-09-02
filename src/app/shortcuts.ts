import type { ShortcutBindingSetting } from "../api/types";
import type { ShortcutDefinition, ShortcutKeyBinding, ShortcutReferenceItem } from "../shared/shortcuts";
import type { Verb } from "../shared/verbs";
import type { Section } from "./navigation";

export type AppShortcutId =
  | "app.openInbox"
  | "app.openCompanies"
  | "app.openWatchlists"
  | "app.openAlerts"
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
  | "company.previousTab";

export type AppShortcutActionMap = Record<AppShortcutId, () => boolean | void>;

export type AppShortcutReferenceItem = ShortcutReferenceItem & {
  id: AppShortcutId;
  defaultBinding: ShortcutKeyBinding;
  disabled: boolean;
  hasCustomBinding: boolean;
  /** The command-palette dictionary verb this shortcut's label starts with
   * (ADR 0104 dec. 3, F3a S3) — consumed when AppShell turns shortcuts into
   * palette commands (`PaletteCommand.verb`). */
  verb: Verb;
};

// Ctrl+4 is unbound in this slice (F4c S2, ADR 0108 amendment): the
// Notebooks-global screen it opened is retired; F4c S3 assigns it to
// Research (`app.openResearch`) — not introduced here.
const navigationShortcuts = [
  ["app.openInbox", "Open Inbox", "1", "Inbox"],
  ["app.openCompanies", "Open Companies", "2", "Companies"],
  ["app.openWatchlists", "Open Watchlists", "3", "Watchlists"],
  ["app.openEvents", "Open Events", "5", "Events"],
  ["app.openTranscripts", "Open Transcripts", "6", "Transcripts"],
  ["app.openSources", "Open Sources", "7", "Sources"],
  ["app.openSettings", "Open Settings", "8", "Settings"],
  ["app.openAlerts", "Open Alerts", "9", "Alerts"],
] satisfies Array<[AppShortcutId, string, string, Section]>;

export const appShortcutReferenceItems: AppShortcutReferenceItem[] = [
  ...navigationShortcuts.map(([id, label, key]) => ({
    id,
    label,
    group: "Navigation",
    scope: "app" as const,
    verb: "open" as const,
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
    verb: "open",
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
    // Verb dictionary (ADR 0104 dec. 3): focusing search is functionally
    // navigating to it — `open`, not a nonexistent "focus" verb.
    label: "Open Inbox search",
    group: "Global actions",
    scope: "app",
    verb: "open",
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
    verb: "refresh",
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
    verb: "refresh",
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
    // List navigation reads as `open` (ADR 0104 dec. 3) — there is no
    // dictionary "select" verb.
    label: "Open next inbox item",
    group: "Inbox",
    scope: "screen",
    verb: "open",
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
    label: "Open previous inbox item",
    group: "Inbox",
    scope: "screen",
    verb: "open",
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
    label: "Mark as read or unread",
    group: "Inbox",
    scope: "screen",
    verb: "markAs",
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
    label: "Mark as saved or unsaved",
    group: "Inbox",
    scope: "screen",
    verb: "markAs",
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
    verb: "open",
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
    label: "Add note from inbox item",
    group: "Inbox",
    scope: "screen",
    verb: "add",
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
    label: "Open next company",
    group: "Companies",
    scope: "screen",
    verb: "open",
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
    label: "Open previous company",
    group: "Companies",
    scope: "screen",
    verb: "open",
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
    verb: "open",
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
    verb: "open",
    binding: {
      key: "H",
    },
    defaultBinding: {
      key: "H",
    },
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
