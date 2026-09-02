import type { AppLocale, ShortcutBindingSetting } from "../../api/types";
import type { AppShortcutReferenceItem } from "../../app/shortcuts";
import { makeTextTranslator } from "../../shared/locale";
import { formatShortcutBinding, type ShortcutKeyBinding } from "../../shared/shortcuts";
import { ActionButton, Checkbox, TextField } from "../../ui";

type ShortcutSettingsProps = {
  locale: AppLocale;
  shortcutBindings: Record<string, ShortcutBindingSetting>;
  shortcutReferences: AppShortcutReferenceItem[];
  onShortcutBindingsChange: (shortcutBindings: Record<string, ShortcutBindingSetting>) => void;
};

function shortcutSignature(binding: ShortcutKeyBinding) {
  return JSON.stringify({
    key: binding.key.toLowerCase(),
    altKey: Boolean(binding.altKey),
    ctrlKey: Boolean(binding.ctrlKey),
    metaKey: Boolean(binding.metaKey),
    shiftKey: Boolean(binding.shiftKey),
  });
}

function normalizeShortcutKey(value: string, fallback: string) {
  const trimmed = value.trim();
  return trimmed ? trimmed.slice(-1).toUpperCase() : fallback;
}

export function ShortcutSettings({
  locale,
  shortcutBindings,
  shortcutReferences,
  onShortcutBindingsChange,
}: ShortcutSettingsProps) {
  const text = makeTextTranslator(locale);
  const groups = Array.from(new Set(shortcutReferences.map((shortcut) => shortcut.group)));
  const activeBindingCounts = shortcutReferences
    .filter((shortcut) => !shortcut.disabled)
    .reduce<Record<string, number>>((counts, shortcut) => {
      const signature = shortcutSignature(shortcut.binding);
      return {
        ...counts,
        [signature]: (counts[signature] ?? 0) + 1,
      };
    }, {});

  function updateShortcut(shortcut: AppShortcutReferenceItem, nextBinding: ShortcutBindingSetting) {
    onShortcutBindingsChange({
      ...shortcutBindings,
      [shortcut.id]: nextBinding,
    });
  }

  function resetShortcut(shortcut: AppShortcutReferenceItem) {
    const nextShortcutBindings = { ...shortcutBindings };
    delete nextShortcutBindings[shortcut.id];
    onShortcutBindingsChange(nextShortcutBindings);
  }

  return (
    <article className="settings-group" aria-labelledby="settings-shortcuts-title">
      <h2 id="settings-shortcuts-title">{text("Keyboard shortcuts")}</h2>
      <p className="settings-note">
        {text("Shortcuts are ignored while typing in fields and editors.")}
      </p>
      <div className="shortcut-reference-list">
        {groups.map((group) => (
          <section className="shortcut-reference-group" key={group} aria-label={text(group)}>
            <h3>{text(group)}</h3>
            <div className="shortcut-reference-grid">
              {shortcutReferences
                .filter((shortcut) => shortcut.group === group)
                .map((shortcut) => {
                  const hasConflict =
                    !shortcut.disabled && activeBindingCounts[shortcutSignature(shortcut.binding)] > 1;
                  const bindingValue: ShortcutBindingSetting = {
                    key: shortcut.binding.key,
                    altKey: shortcut.binding.altKey,
                    ctrlKey: shortcut.binding.ctrlKey,
                    metaKey: shortcut.binding.metaKey,
                    shiftKey: shortcut.binding.shiftKey,
                    disabled: shortcut.disabled,
                  };

                  return (
                    <div
                      className={hasConflict ? "shortcut-reference-row shortcut-reference-row-conflict" : "shortcut-reference-row"}
                      key={shortcut.id}
                    >
                      <div className="shortcut-reference-label">
                        <span>{text(shortcut.label)}</span>
                        <kbd>{formatShortcutBinding(shortcut.binding)}</kbd>
                      </div>
                      <div className="shortcut-reference-controls" aria-label={`${text("Configure shortcut")} ${text(shortcut.label)}`}>
                        <Checkbox
                          checked={!shortcut.disabled}
                          onChange={(event) => {
                            updateShortcut(shortcut, {
                              ...bindingValue,
                              disabled: !event.target.checked,
                            });
                          }}
                          label={text("Enabled")}
                        />
                        <TextField
                          label={text("Key")}
                          aria-label={`${text("Shortcut key")} ${text(shortcut.label)}`}
                          disabled={shortcut.disabled}
                          maxLength={1}
                          onFocus={(event) => event.target.select()}
                          onChange={(event) => {
                            updateShortcut(shortcut, {
                              ...bindingValue,
                              key: normalizeShortcutKey(event.target.value, shortcut.defaultBinding.key),
                            });
                          }}
                          value={shortcut.binding.key}
                        />
                        {(["ctrlKey", "altKey", "shiftKey", "metaKey"] as const).map((modifier) => (
                          <Checkbox
                            key={modifier}
                            checked={Boolean(shortcut.binding[modifier])}
                            disabled={shortcut.disabled}
                            onChange={(event) => {
                              updateShortcut(shortcut, {
                                ...bindingValue,
                                [modifier]: event.target.checked,
                              });
                            }}
                            label={text(modifier)}
                          />
                        ))}
                        <ActionButton
                          kind="control"
                          disabled={!shortcut.hasCustomBinding}
                          onClick={() => resetShortcut(shortcut)}
                        >
                          {text("Reset")}
                        </ActionButton>
                      </div>
                      {hasConflict ? (
                        <p className="settings-note shortcut-conflict-note">
                          {text("Shortcut conflicts with another enabled action.")}
                        </p>
                      ) : null}
                    </div>
                  );
                })}
            </div>
          </section>
        ))}
      </div>
    </article>
  );
}
