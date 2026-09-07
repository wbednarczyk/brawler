import { useEffect, useRef } from "react";
import { Modal, SearchField, useComboboxListbox } from "../../ui";
import { focusScreenHeadingIfBody } from "../focus/focusScreenHeading";
import type { Verb } from "../verbs";

// Shared command palette — the keyboard-first launcher (⌘K). A self-contained,
// controlled presentation unit: it renders a filtered, arrow/enter-navigable
// list of {id,label,run} commands inside a Modal. The command set is supplied
// by the caller — the global palette (src/app/commandPalette.tsx, mounted in
// AppShell) feeds it the merged app + contextual list. Filter + keyboard nav
// live here.

// `actionKey`/`verb` (ADR 0104 dec. 3, F3a S3): stable, label-independent
// identity for a command plus its dictionary verb — the copy gate
// (src/app/paletteCopy.test.ts) checks every producer's labels against
// `verb` and that no two verbs share one `actionKey`.
export type PaletteCommand = { id: string; label: string; run: () => void; actionKey: string; verb: Verb };

function filterCommand(command: PaletteCommand, query: string): boolean {
  return command.label.toLowerCase().includes(query.toLowerCase());
}

export function CommandPalette({
  open,
  commands,
  onClose,
  text,
}: {
  open: boolean;
  commands: PaletteCommand[];
  onClose: () => void;
  text: (s: string) => string;
}) {
  const inputRef = useRef<HTMLInputElement>(null);

  function runCommand(command: PaletteCommand) {
    command.run();
    onClose();
    // The Modal restores focus to the invoker on unmount; when that invoker
    // was `<body>` (Ctrl+K from nowhere) or left with the previous screen,
    // land on the new screen's heading instead (never `<body>`).
    requestAnimationFrame(() => {
      focusScreenHeadingIfBody();
    });
  }

  // Driven by the SAME headless controller the Spółka company picker uses
  // (dogfooding wave 2026-09, #3) — command execution, the Modal's own
  // lifecycle, the empty state and this file's CSS stay palette-specific;
  // filtering/keyboard-nav/activedescendant live in the shared hook. Policy
  // "close-host": ONE Escape always closes the modal and restores the
  // invoker — never a closed-list-but-open-modal state (the palette has no
  // "closed list" state of its own to begin with).
  const controller = useComboboxListbox({
    options: commands,
    getId: (command) => command.id,
    filter: filterCommand,
    onSelect: runCommand,
    escapePolicy: () => "close-host",
    onCloseHost: onClose,
  });

  // Reset to a blank, first-option-active, open list on every open (matches
  // the previous `setQuery("")`/`setActive(0)` reset) — `CommandPalette`
  // itself stays mounted across opens (only `Modal`'s own render toggles),
  // so the controller's state would otherwise carry over from the last use.
  useEffect(() => {
    if (open) {
      controller.reset();
      controller.open();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- fires only on an `open` transition; `controller`'s methods read current state at call time
  }, [open]);

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={text("Command palette")}
      ariaLabel={text("Command palette")}
      initialFocusRef={inputRef}
    >
      <div className="command-palette">
        <SearchField
          ariaLabel={text("Search commands")}
          className="search-box"
          placeholder={text("Type to filter commands…")}
          value={controller.query}
          onChange={(value) => controller.setQuery(value)}
          inputProps={{
            ref: inputRef,
            role: controller.inputProps.role,
            "aria-expanded": controller.inputProps["aria-expanded"],
            "aria-autocomplete": controller.inputProps["aria-autocomplete"],
            "aria-controls": controller.inputProps["aria-controls"],
            "aria-activedescendant": controller.inputProps["aria-activedescendant"],
            onKeyDown: controller.inputProps.onKeyDown,
          }}
        />
        <ul {...controller.listboxProps} className="command-palette-list" aria-label={text("Commands")}>
          {controller.filtered.length === 0 ? (
            <li className="command-palette-empty">{text("No matching commands.")}</li>
          ) : null}
          {controller.filtered.map((command) => {
            const optionProps = controller.optionProps(command);
            return (
              <li
                key={command.id}
                {...optionProps}
                className={["command-palette-item", optionProps["aria-selected"] ? "is-active" : ""]
                  .filter(Boolean)
                  .join(" ")}
              >
                {command.label}
              </li>
            );
          })}
        </ul>
      </div>
    </Modal>
  );
}
