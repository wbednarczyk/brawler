import { describe, it } from "vitest";
import { appTestState, expect, renderApp, screen, userEvent, within } from "../../test/appWorkflowHarness";
import {
  collectActionInventory,
  expectPrimaryMarkerMatchesVariant,
  expectSinglePrimary,
} from "../../test/uxContracts";
import type { ActionInventoryEntry } from "../../test/uxContracts";
import { appShortcutReferenceItems } from "../../app/shortcuts";

// F4c S1 (docs/plans/f4c-contracts/s1-guardrails.md item 4, plan §
// Decisions 5): RED contract skeleton for the Settings language pass — S4
// makes every control an `ActionButton` carrying the labels/kinds this table
// names; this file pins the target shape. Shape mirrors
// SourcesScreen.contract.test.tsx (F4b S4), "contract-exempt short form" per
// dec. 5: one state per tab (10 tabs), not the full substate matrix Research
// needed. Every state fails TODAY on both axes: every button's
// `data-action-kind` is `"unclassified"` and several labels are pre-F4c
// (`Stdio adapter` instead of `Claude Code (terminal)`, etc.).
//
// The Subnav tab buttons (Appearance/Sources/…/MCP, unchanged labels per
// dec. 5, `kind="control"` per s1-guardrails item 4) render on EVERY state
// regardless of which tab is active — `SettingsScreen.tsx` (`Subnav`) always
// renders all of them; only the section select's `<select>` (not a button)
// hides at wider tiers. The Transcripts and Credentials tabs retired with
// video transcription (ADR 0111) — seven tabs remain.
//
// ASSUMPTIONS not settled verbatim by the plan (S4 owns the final call):
// (a) which tab(s) are "the token composers" carrying the single
// primary at rest — read as MCP (the access-token Generate — NOT the
// acquisition-token Generate, since a screen carries one primary); the other tabs
// assert `expectSinglePrimary(region, 0)`. (b) `kind` for controls the plan
// doesn't name explicitly: the four DB/Queue/MCP "Reset…" buttons and the
// per-shortcut "Reset" button → `control` (no dictionary verb fits a
// revert-to-default action); Import/Export actions → `Export` = `fetch`,
// `Import` (opens the file picker) = `open`, `Apply import` = `apply`
// (exact dictionary match).
//
// S4 DEVIATION (stated reason, per f4c-common.md "adjust expected values
// only with a stated reason"): `copyTerminal` corrected from "Copy — Claude
// Code (terminal)" to "Copy — Bridge command" — the sol R2 amendment (S4
// contract "Amendments after sol R2") supersedes the skeleton's plan-dec.-5
// wording: the stdio snippet is a process invocation the assistant launches
// itself, not equivalent to `claude mcp add`, so labeling it "Claude Code"
// would misrepresent it.

const LOCALES = ["en", "pl"] as const;
type Locale = (typeof LOCALES)[number];

const APP_SETTINGS_LABEL = { en: "Application settings", pl: "Ustawienia aplikacji" } as const;

function sorted(entries: ActionInventoryEntry[]): ActionInventoryEntry[] {
  return [...entries].sort(
    (a, b) => a.name.localeCompare(b.name, "en") || a.kind.localeCompare(b.kind, "en"),
  );
}

const TAB_LABELS = {
  en: {
    appearance: "Appearance",
    sources: "Sources",
    importExport: "Import And Export",
    shortcuts: "Keyboard shortcuts",
    logs: "Logs",
    database: "Data storage",
    mcp: "MCP server",
  },
  pl: {
    appearance: "Wygląd",
    sources: "Źródła",
    importExport: "Import i eksport",
    shortcuts: "Skróty klawiaturowe",
    logs: "Logi",
    database: "Przechowywanie danych",
    mcp: "Serwer MCP",
  },
} as const;

const LABELS = {
  en: {
    resetToDefaults: "Reset to defaults",
    resetToDefaultPort: "Reset to default port",
    generateToken: "Generate token",
    generateAcquisitionToken: "Generate report-data token",
    copyHttp: "Copy — Claude Code (HTTP)",
    copyTerminal: "Copy — Bridge command",
    export: "Export",
    import: "Import",
    reset: "Reset",
    createBackup: "Create backup",
    refresh: "Refresh",
    restore: "Restore",
  },
  pl: {
    resetToDefaults: "Przywróć domyślne",
    resetToDefaultPort: "Przywróć domyślny port",
    generateToken: "Wygeneruj token",
    generateAcquisitionToken: "Wygeneruj token do danych raportów",
    copyHttp: "Kopiuj — Claude Code (HTTP)",
    copyTerminal: "Kopiuj — Polecenie mostka",
    export: "Eksport",
    import: "Import",
    reset: "Resetuj",
    createBackup: "Utwórz kopię",
    refresh: "Odśwież",
    restore: "Przywróć",
  },
} as const;

function tabNavInventory(locale: Locale): ActionInventoryEntry[] {
  const t = TAB_LABELS[locale];
  return Object.values(t).map((name) => ({ name, kind: "control" }));
}

const SOURCE_PRESETS = ["1", "3", "5", "10"];

async function openSettingsTab(locale: Locale, tab: keyof (typeof TAB_LABELS)["en"]) {
  appTestState.settingsResponse = { ...appTestState.settingsResponse, locale };
  renderApp({ section: "Settings" });
  const region = await screen.findByLabelText(APP_SETTINGS_LABEL[locale]);
  const user = userEvent.setup();
  await user.click(within(region).getByRole("button", { name: TAB_LABELS[locale][tab] }));
  return region;
}

describe("Settings action inventory (F4c contract § Settings, plan dec. 5)", () => {
  it.each(LOCALES)("Appearance tab: tab nav only, no content actions (%s)", async (locale) => {
    const region = await openSettingsTab(locale, "appearance");
    expect(collectActionInventory(region, locale)).toEqual(sorted(tabNavInventory(locale)));
    expectPrimaryMarkerMatchesVariant(region);
    expectSinglePrimary(region, 0);
  });

  it.each(LOCALES)("Sources tab: backfill-depth presets (%s)", async (locale) => {
    const region = await openSettingsTab(locale, "sources");
    expect(collectActionInventory(region, locale)).toEqual(
      sorted([...tabNavInventory(locale), ...SOURCE_PRESETS.map((name) => ({ name, kind: "control" }))]),
    );
    expectPrimaryMarkerMatchesVariant(region);
    expectSinglePrimary(region, 0);
  });

  it.each(LOCALES)("Import and export tab: Export/Import per panel, no preview yet (%s)", async (locale) => {
    const t = LABELS[locale];
    const region = await openSettingsTab(locale, "importExport");
    expect(collectActionInventory(region, locale)).toEqual(
      sorted([
        ...tabNavInventory(locale),
        { name: t.export, kind: "fetch" },
        { name: t.export, kind: "fetch" },
        { name: t.import, kind: "open" },
        { name: t.import, kind: "open" },
      ]),
    );
    expectPrimaryMarkerMatchesVariant(region);
    expectSinglePrimary(region, 0);
  });

  it.each(LOCALES)(
    "Keyboard shortcuts tab: one Reset per bound shortcut (%s)",
    async (locale) => {
      const t = LABELS[locale];
      const region = await openSettingsTab(locale, "shortcuts");
      const resetCount = appShortcutReferenceItems.length;
      expect(collectActionInventory(region, locale)).toEqual(
        sorted([
          ...tabNavInventory(locale),
          ...Array.from({ length: resetCount }, () => ({ name: t.reset, kind: "control" })),
        ]),
      );
      expectPrimaryMarkerMatchesVariant(region);
      expectSinglePrimary(region, 0);
    },
  );

  it.each(LOCALES)("Logs tab: tab nav only, no content actions (%s)", async (locale) => {
    const region = await openSettingsTab(locale, "logs");
    expect(collectActionInventory(region, locale)).toEqual(sorted(tabNavInventory(locale)));
    expectPrimaryMarkerMatchesVariant(region);
    expectSinglePrimary(region, 0);
  });

  it.each(LOCALES)(
    // #451: Backups moved from developer-gated Diagnostics into this tab, as
    // its own settings-group under the connection-pool group — Create backup
    // (verb create), Refresh (control), one Restore (control) per seeded
    // backup row (the sample scenario always seeds 2: a rotating backup and a
    // pre-migration snapshot, EMPTY_SINGLETONS.backupStatus).
    "Data storage tab: Database + Queue resets, plus Backups actions (%s)",
    async (locale) => {
      const t = LABELS[locale];
      const region = await openSettingsTab(locale, "database");
      // Backups load async (own effect, not part of the settings bootstrap
      // payload) — wait for the seeded rows before reading the inventory.
      await within(region).findAllByRole("button", { name: t.restore });
      expect(collectActionInventory(region, locale)).toEqual(
        sorted([
          ...tabNavInventory(locale),
          { name: t.resetToDefaults, kind: "control" },
          { name: t.resetToDefaults, kind: "control" },
          { name: t.createBackup, kind: "create" },
          { name: t.refresh, kind: "control" },
          { name: t.restore, kind: "control" },
          { name: t.restore, kind: "control" },
        ]),
      );
      expectPrimaryMarkerMatchesVariant(region);
      expectSinglePrimary(region, 1);
    },
  );

  it.each(LOCALES)(
    // #451 sol R2 correction: exactly one Create-backup action lives in the
    // section at all times (ADR 0104 dec. 4) — with no backups it moves into
    // the empty-state invitation instead of the toolbar, so the inventory
    // must stay the same shape (Create backup + Refresh, zero Restore rows).
    "Data storage tab: Backups empty state still carries Create backup + Refresh, no Restore (%s)",
    async (locale) => {
      const t = LABELS[locale];
      appTestState.backupStatusResponse = { lastBackupAt: null, backupCount: 0, backups: [] };
      const region = await openSettingsTab(locale, "database");
      await screen.findByText(
        locale === "pl" ? "Lokalne kopie Twoich danych." : "Local copies of your data.",
      );
      expect(collectActionInventory(region, locale)).toEqual(
        sorted([
          ...tabNavInventory(locale),
          { name: t.resetToDefaults, kind: "control" },
          { name: t.resetToDefaults, kind: "control" },
          { name: t.createBackup, kind: "create" },
          { name: t.refresh, kind: "control" },
        ]),
      );
      expectPrimaryMarkerMatchesVariant(region);
      expectSinglePrimary(region, 1);
    },
  );

  it.each(LOCALES)(
    "MCP tab: Generate token is the primary token composer at rest, not configured (%s)",
    async (locale) => {
      const t = LABELS[locale];
      const region = await openSettingsTab(locale, "mcp");
      expect(collectActionInventory(region, locale)).toEqual(
        sorted([
          ...tabNavInventory(locale),
          { name: t.resetToDefaultPort, kind: "control" },
          { name: t.generateToken, kind: "create" },
          { name: t.generateAcquisitionToken, kind: "create" },
          { name: t.copyHttp, kind: "control" },
          { name: t.copyTerminal, kind: "control" },
        ]),
      );
      expectPrimaryMarkerMatchesVariant(region);
      expectSinglePrimary(region, 1);
      const primary = region.querySelector('[data-ux-primary-action="true"]');
      expect(primary).toHaveTextContent(t.generateToken);
    },
  );

  it("no button in the screen root is left unclassified — every action is now classified", async () => {
    const region = await openSettingsTab("en", "mcp");
    const unclassified = collectActionInventory(region, "en").filter(
      (entry) => entry.kind === "unclassified",
    );
    expect(unclassified).toEqual([]);
  });
});
