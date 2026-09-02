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
// The ten Subnav tab buttons (Appearance/Sources/…/License, unchanged labels
// per dec. 5, `kind="control"` per s1-guardrails item 4) render on EVERY
// state regardless of which tab is active — `SettingsScreen.tsx:141-147`
// (`Subnav`) always renders all ten; only the section select's `<select>`
// (not a button) hides at wider tiers.
//
// ASSUMPTIONS not settled verbatim by the plan (S4 owns the final call):
// (a) which tab(s) are "the token/license composers" carrying the single
// primary at rest — read as Credentials (the Gemini API key save), MCP (the
// access-token Generate — NOT the acquisition-token Generate, since a screen
// carries one primary), and License (Save license); the other seven tabs
// assert `expectSinglePrimary(region, 0)`. (b) `kind` for controls the plan
// doesn't name explicitly: the four DB/Queue/MCP "Reset…" buttons and the
// per-shortcut "Reset" button → `control` (no dictionary verb fits a
// revert-to-default action); Import/Export actions → `Export` = `fetch`,
// `Import` (opens the file picker) = `open`, `Apply import` = `apply`
// (exact dictionary match). (c) Credentials' "Clear" (wipes the stored/draft
// credential) → `remove`.
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
    transcripts: "Transcripts",
    credentials: "Credentials",
    importExport: "Import And Export",
    shortcuts: "Keyboard shortcuts",
    logs: "Logs",
    database: "Data storage",
    mcp: "MCP server",
    license: "License",
  },
  pl: {
    appearance: "Wygląd",
    sources: "Źródła",
    transcripts: "Transkrypcje",
    credentials: "Poświadczenia",
    importExport: "Import i eksport",
    shortcuts: "Skróty klawiaturowe",
    logs: "Logi",
    database: "Przechowywanie danych",
    mcp: "Serwer MCP",
    license: "Licencja",
  },
} as const;

const LABELS = {
  en: {
    save: "Save",
    clear: "Clear",
    getGeminiKey: "Get Gemini API key",
    resetToDefaults: "Reset to defaults",
    resetToDefaultPort: "Reset to default port",
    generateToken: "Generate token",
    generateAcquisitionToken: "Generate report-data token",
    copyHttp: "Copy — Claude Code (HTTP)",
    copyTerminal: "Copy — Bridge command",
    saveLicense: "Save license",
    clearLicense: "Clear license",
    export: "Export",
    import: "Import",
    reset: "Reset",
  },
  pl: {
    save: "Zapisz",
    clear: "Wyczyść",
    getGeminiKey: "Pobierz klucz API Gemini",
    resetToDefaults: "Przywróć domyślne",
    resetToDefaultPort: "Przywróć domyślny port",
    generateToken: "Wygeneruj token",
    generateAcquisitionToken: "Wygeneruj token do danych raportów",
    copyHttp: "Kopiuj — Claude Code (HTTP)",
    copyTerminal: "Kopiuj — Polecenie mostka",
    saveLicense: "Zapisz licencję",
    clearLicense: "Wyczyść licencję",
    export: "Eksport",
    import: "Import",
    reset: "Resetuj",
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

  it.each(LOCALES)("Transcripts tab: tab nav only, no content actions (%s)", async (locale) => {
    const region = await openSettingsTab(locale, "transcripts");
    expect(collectActionInventory(region, locale)).toEqual(sorted(tabNavInventory(locale)));
    expectPrimaryMarkerMatchesVariant(region);
    expectSinglePrimary(region, 0);
  });

  it.each(LOCALES)(
    "Credentials tab: Save is the primary token composer at rest (%s)",
    async (locale) => {
      const t = LABELS[locale];
      const region = await openSettingsTab(locale, "credentials");
      expect(collectActionInventory(region, locale)).toEqual(
        sorted([
          ...tabNavInventory(locale),
          { name: t.save, kind: "save" },
          { name: t.clear, kind: "remove" },
          { name: t.getGeminiKey, kind: "open" },
        ]),
      );
      expectPrimaryMarkerMatchesVariant(region);
      expectSinglePrimary(region, 1);
      const primary = region.querySelector('[data-ux-primary-action="true"]');
      expect(primary).toHaveTextContent(t.save);
    },
  );

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
    "Data storage tab: Database + Queue reset controls (%s)",
    async (locale) => {
      const t = LABELS[locale];
      const region = await openSettingsTab(locale, "database");
      expect(collectActionInventory(region, locale)).toEqual(
        sorted([
          ...tabNavInventory(locale),
          { name: t.resetToDefaults, kind: "control" },
          { name: t.resetToDefaults, kind: "control" },
        ]),
      );
      expectPrimaryMarkerMatchesVariant(region);
      expectSinglePrimary(region, 0);
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

  it.each(LOCALES)(
    "License tab: Save license is the primary composer at rest (%s)",
    async (locale) => {
      const t = LABELS[locale];
      const region = await openSettingsTab(locale, "license");
      expect(collectActionInventory(region, locale)).toEqual(
        sorted([
          ...tabNavInventory(locale),
          { name: t.saveLicense, kind: "save" },
          { name: t.clearLicense, kind: "remove" },
        ]),
      );
      expectPrimaryMarkerMatchesVariant(region);
      expectSinglePrimary(region, 1);
      const primary = region.querySelector('[data-ux-primary-action="true"]');
      expect(primary).toHaveTextContent(t.saveLicense);
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
