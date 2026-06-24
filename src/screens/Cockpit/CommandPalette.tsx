import { useEffect, useState } from "react";
import { Modal, SearchField } from "../../ui";

// Cockpit command palette (ADR 0053, phase 3) — the keyboard-first panel
// launcher (⌘K). Commands are supplied by the cockpit: open a company panel,
// re-show a closed linked panel, load a saved layout, or reset. Extracted from
// CockpitScreen so the launcher is a self-contained, testable unit and can be
// shared with the v0.48.0 global command-palette work. Filter + arrow/enter
// keyboard nav live here; the command set lives with the cockpit.

export type PaletteCommand = { id: string; label: string; run: () => void };

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
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  useEffect(() => {
    if (open) {
      setQuery("");
      setActive(0);
    }
  }, [open]);

  const filtered = query.trim()
    ? commands.filter((command) => command.label.toLowerCase().includes(query.trim().toLowerCase()))
    : commands;
  const clampedActive = Math.min(active, Math.max(0, filtered.length - 1));

  function run(index: number) {
    const command = filtered[index];
    if (command) {
      command.run();
      onClose();
    }
  }

  return (
    <Modal open={open} onClose={onClose} title={text("Command palette")} ariaLabel={text("Command palette")}>
      <div className="cockpit-palette">
        <SearchField
          ariaLabel={text("Search commands")}
          placeholder={text("Type to filter commands…")}
          value={query}
          onChange={(value) => {
            setQuery(value);
            setActive(0);
          }}
          inputProps={{
            autoFocus: true,
            onKeyDown: (event) => {
              if (event.key === "ArrowDown") {
                event.preventDefault();
                setActive((index) => Math.min(index + 1, filtered.length - 1));
              } else if (event.key === "ArrowUp") {
                event.preventDefault();
                setActive((index) => Math.max(index - 1, 0));
              } else if (event.key === "Enter") {
                event.preventDefault();
                run(clampedActive);
              }
            },
          }}
        />
        <ul className="cockpit-palette-list">
          {filtered.length === 0 ? (
            <li className="cockpit-palette-empty">{text("No matching commands.")}</li>
          ) : null}
          {filtered.map((command, index) => (
            <li key={command.id}>
              <button
                type="button"
                className={["cockpit-palette-item", index === clampedActive ? "is-active" : ""]
                  .filter(Boolean)
                  .join(" ")}
                onClick={() => run(index)}
                onMouseEnter={() => setActive(index)}
              >
                {command.label}
              </button>
            </li>
          ))}
        </ul>
      </div>
    </Modal>
  );
}
