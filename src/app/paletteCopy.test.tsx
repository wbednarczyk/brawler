import { describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { buildAppCommands, type PinnedCompany } from "./AppShell";
import { appShortcutReferenceItems, type AppShortcutActionMap } from "./shortcuts";
import { CommandPaletteProvider, useCommandPalette, useCommandPaletteCommands, type PaletteCommand } from "./commandPalette";
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
    onOpenActivity: () => {},
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

  // F3c S2 (#197): "Today" joins the global-screen palette entries
  // (SCREEN_PALETTE_ENTRIES) alongside Research/Events/Report Season.
  for (const locale of ["en", "pl"] as const) {
    it(`lists the Today screen entry (${locale})`, () => {
      const text = makeTextTranslator(locale);
      const commands = collectCommands(locale);
      const today = commands.find((command) => command.actionKey === "screen.open.today");
      expect(today).toBeDefined();
      expect(today!.label).toBe(`${text("Open screen")}: ${text("Today")}`);
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

// #454: on Spółka the palette merges THREE same-topic families — the Ctrl+N
// nav shortcut ("Open Events"), the global "Open screen: <Name>" entry
// (Events also has a Library nav item), and the Spółka contextual
// "Open tool: <Name>" entry. Before the `tool:` family the tool command read
// "Open events" (lower e) — a same-prefix collision with "Open Events" that
// made the palette's substring filter pick the wrong one on Enter. This pins
// all three staying textually distinct AND wired to the right `run`.
function OpenPaletteButton() {
  const { open } = useCommandPalette();
  return (
    <button type="button" onClick={open}>
      launch
    </button>
  );
}

function SpolkaToolContributor({ onRun }: { onRun: (actionKey: string) => void }) {
  const commands: PaletteCommand[] = SPOLKA_TOOL_COMMANDS.map(({ actionKey, label }) => ({
    id: `spolka-tool:${actionKey}`,
    label,
    verb: "open",
    actionKey,
    run: () => onRun(actionKey),
  }));
  useCommandPaletteCommands("spolka", commands);
  return null;
}

describe("palette disambiguation on Spółka: nav shortcut vs 'Open screen:' vs 'Open tool:' (#454)", () => {
  it("each query resolves to exactly one command, and Enter runs the right one", async () => {
    const user = userEvent.setup();
    const setActiveSection = vi.fn();
    const openEventsShortcut = vi.fn();
    const toolRun = vi.fn();
    const shortcutActionMap = Object.fromEntries(
      appShortcutReferenceItems.map((item) => [item.id, () => {}]),
    ) as AppShortcutActionMap;
    shortcutActionMap["app.openEvents"] = openEventsShortcut;

    const appCommands = buildAppCommands({
      shortcutBindings: {},
      shortcutActionMap,
      trackedCompanies: [],
      onOpenCompany: () => {},
      setActiveSection,
      onOpenActivity: () => {},
      text: (s) => s,
    });

    render(
      <CommandPaletteProvider appCommands={appCommands} text={(s) => s}>
        <OpenPaletteButton />
        <SpolkaToolContributor onRun={toolRun} />
      </CommandPaletteProvider>,
    );

    async function runQuery(query: string) {
      await user.click(screen.getByRole("button", { name: "launch" }));
      const dialog = screen.getByRole("dialog", { name: "Command palette" });
      await user.type(within(dialog).getByRole("combobox", { name: "Search commands" }), query);
      const options = within(dialog).getAllByRole("option");
      const labels = options.map((option) => option.textContent);
      await user.click(options[0]!);
      return labels;
    }

    expect(await runQuery("open events")).toEqual(["Open Events"]);
    expect(openEventsShortcut).toHaveBeenCalledTimes(1);
    expect(setActiveSection).not.toHaveBeenCalled();
    expect(toolRun).not.toHaveBeenCalled();

    expect(await runQuery("screen: ev")).toEqual(["Open screen: Events"]);
    expect(setActiveSection).toHaveBeenCalledWith("Events");
    expect(toolRun).not.toHaveBeenCalled();

    expect(await runQuery("tool: ev")).toEqual(["Open tool: Events"]);
    expect(toolRun).toHaveBeenCalledWith("tool.open.wydarzenia");
    expect(openEventsShortcut).toHaveBeenCalledTimes(1);
  });
});
