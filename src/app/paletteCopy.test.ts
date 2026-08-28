import { describe, expect, it } from "vitest";

import { buildAppCommands, type PinnedCompany } from "./AppShell";
import { appShortcutReferenceItems, type AppShortcutActionMap } from "./shortcuts";
import { SPOLKA_TOOL_COMMANDS } from "../screens/Spolka/SpolkaScreen";
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
// "Paleta z metadanymi"): AppShell's app-level commands (shortcuts, every
// tracked company, the global screens) plus the Spółka screen's contextual
// tool-open commands. The rich fixture (several tracked companies) mirrors
// what a populated app actually renders.
function collectCommands(locale: LocaleCode): Command[] {
  const text = makeTextTranslator(locale);
  const appCommands = buildAppCommands({
    shortcutBindings: {},
    shortcutActionMap: noopShortcutActions,
    trackedCompanies,
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
