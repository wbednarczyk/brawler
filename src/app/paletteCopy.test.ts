import { describe, expect, it } from "vitest";

import { buildAppCommands, type PinnedCompany } from "./AppShell";
import { appShortcutReferenceItems, type AppShortcutActionMap } from "./shortcuts";
import { SPOLKA_TOOL_COMMANDS } from "../screens/Spolka/SpolkaScreen";
import { buildCockpitCommands } from "../screens/Cockpit/CockpitScreen";
import type { CockpitLayout } from "../api/cockpit";
import { VERB_LABELS, type Verb } from "../shared/verbs";
import { makeTextTranslator, type LocaleCode } from "../shared/locale";
import { COMPANY_SPECS, makeCompany } from "../test/scenarios/entities";

// A no-op action for every registered shortcut id.
const noopShortcutActions = Object.fromEntries(
  appShortcutReferenceItems.map((item) => [item.id, () => {}]),
) as AppShortcutActionMap;

const trackedCompanies: PinnedCompany[] = COMPANY_SPECS.slice(0, 3).map((spec) => {
  const company = makeCompany(spec);
  return { id: company.id, name: company.displayName, ticker: company.ticker };
});

type Command = { id: string; label: string; actionKey: string; verb: Verb };

// Every palette command the app can produce (ADR 0104 dec. 3, F3a S3, plan
// "Paleta z metadanymi"): AppShell's app-level commands (shortcuts, saved
// views, every tracked company, the global screens) plus the Spółka screen's
// contextual tool-open commands. The rich fixture (a saved view + several
// tracked companies) mirrors what a populated app actually renders.
//
// The frozen cockpit's OWN local commands (CockpitScreen.tsx) are
// deliberately excluded — plan "Separacja słowników": the cockpit's palette
// carries no shared vocabulary obligation with Spółka/global this slice
// (S3b's job when the cockpit freeze itself is finished).
function collectCommands(locale: LocaleCode): Command[] {
  const text = makeTextTranslator(locale);
  const appCommands = buildAppCommands({
    shortcutBindings: {},
    shortcutActionMap: noopShortcutActions,
    cockpitViews: [{ id: "view_1", name: "Deep dive" }],
    trackedCompanies,
    onOpenCockpitView: () => {},
    onOpenCompany: () => {},
    setActiveSection: () => {},
    text,
  });
  const spolkaToolCommands: Command[] = SPOLKA_TOOL_COMMANDS.map(({ actionKey, label }) => ({
    id: `spolka-tool:${actionKey}`,
    label: text(label),
    verb: "open",
    actionKey,
  }));
  return [...appCommands, ...spolkaToolCommands];
}

describe("palette copy gate (ADR 0104 dec. 3, F3a S3)", () => {
  for (const locale of ["en", "pl"] as const) {
    it(`every command label starts with its dictionary verb, both locales (${locale})`, () => {
      const commands = collectCommands(locale);
      // A sanity floor so a future refactor that accidentally empties the
      // command list can't pass this gate vacuously.
      expect(commands.length).toBeGreaterThan(20);

      const offenders = commands.filter((command) => {
        const verbLabel = VERB_LABELS[command.verb][locale];
        return !(command.label.startsWith(`${verbLabel} `) || command.label.startsWith(`${verbLabel}:`));
      });
      expect(
        offenders.map((command) => `${command.actionKey} [${command.verb}]: "${command.label}"`),
      ).toEqual([]);
    });

    it(`no command label is a full sentence, both locales (${locale})`, () => {
      const commands = collectCommands(locale);
      const offenders = commands.filter((command) => command.label.trim().endsWith("."));
      expect(offenders.map((command) => command.actionKey)).toEqual([]);
    });
  }

  // sol R1 finding 11: the cockpit's OWN commands are deliberately excluded
  // from `collectCommands` (no shared VOCABULARY with Spółka/global, per
  // "Separacja słowników" above) — but that exclusion left them untested
  // against the universal STRUCTURAL copy rules (verb prefix, no full
  // sentence) in either locale. A separate, narrow check: same two rules,
  // both locales, without merging cockpit's actionKeys into the shared
  // corpus the disjointness test below still guards.
  for (const locale of ["en", "pl"] as const) {
    it(`cockpit command labels start with their verb and are not full sentences (${locale})`, () => {
      const text = makeTextTranslator(locale);
      const namedLayouts: CockpitLayout[] = [
        {
          id: "view_1",
          name: "Deep dive",
          ordinal: 0,
          panelsJson: "{}",
          layoutJson: null,
          dockviewVersion: null,
          createdAt: "",
          updatedAt: "",
        },
      ];
      const cockpitCommands = buildCockpitCommands(text, namedLayouts, () => {});
      expect(cockpitCommands.length).toBeGreaterThan(0);

      const verbLabel = VERB_LABELS.open[locale];
      const offenders = cockpitCommands.filter(
        (command) =>
          !(command.label.startsWith(`${verbLabel} `) || command.label.startsWith(`${verbLabel}:`)) ||
          command.label.trim().endsWith("."),
      );
      expect(offenders.map((command) => `${command.actionKey}: "${command.label}"`)).toEqual([]);
    });
  }

  // F3a S3 (ADR 0107 decision 5, plan § Separacja słowników): the frozen
  // cockpit's local palette is navigation-only ("Open view: …") — it must
  // never carry a Spółka/global dictionary entry, and the Spółka palette must
  // never carry a cockpit entry.
  it("cockpit palette is navigation-only and disjoint from the Spółka dictionary", () => {
    const namedLayouts: CockpitLayout[] = [
      {
        id: "view_1",
        name: "Deep dive",
        ordinal: 0,
        panelsJson: "{}",
        layoutJson: null,
        dockviewVersion: null,
        createdAt: "",
        updatedAt: "",
      },
    ];
    const text = makeTextTranslator("en");
    const cockpitCommands = buildCockpitCommands(text, namedLayouts, () => {});

    expect(cockpitCommands.length).toBeGreaterThan(0);
    for (const command of cockpitCommands) {
      expect(command.actionKey).toBe("view.open");
      expect(command.verb).toBe("open");
    }

    const spolkaActionKeys = new Set(SPOLKA_TOOL_COMMANDS.map((c) => c.actionKey));
    const appActionKeys = new Set(
      buildAppCommands({
        shortcutBindings: {},
        shortcutActionMap: noopShortcutActions,
        cockpitViews: [{ id: "view_1", name: "Deep dive" }],
        trackedCompanies,
        onOpenCockpitView: () => {},
        onOpenCompany: () => {},
        setActiveSection: () => {},
        text,
      }).map((c) => c.actionKey),
    );
    for (const command of cockpitCommands) {
      expect(spolkaActionKeys.has(command.actionKey)).toBe(false);
      // `view.open` is the cockpit's own actionKey — the app-level "Open view:
      // …" sidebar command uses a per-layout-id key, so there is no collision.
      expect(appActionKeys.has(command.actionKey)).toBe(false);
    }
  });

  it("no two distinct verbs share one actionKey", () => {
    const byActionKey = new Map<string, Verb>();
    const collisions: string[] = [];
    for (const command of collectCommands("en")) {
      const existing = byActionKey.get(command.actionKey);
      if (existing === undefined) {
        byActionKey.set(command.actionKey, command.verb);
      } else if (existing !== command.verb) {
        collisions.push(`${command.actionKey}: ${existing} vs ${command.verb}`);
      }
    }
    expect(collisions).toEqual([]);
  });
});
