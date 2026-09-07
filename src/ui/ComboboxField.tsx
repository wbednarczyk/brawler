import { forwardRef, useId, useRef, useState, type ReactElement, type ReactNode, type Ref } from "react";

import { useLocale } from "../shared/locale";
import { useComboboxListbox, type ComboboxEscapeAction, type ComboboxEscapeState } from "./useComboboxListbox";

function mergeRefs<T>(...refs: Array<Ref<T> | undefined>): (node: T | null) => void {
  return (node) => {
    for (const ref of refs) {
      if (!ref) continue;
      if (typeof ref === "function") ref(node);
      else (ref as { current: T | null }).current = node;
    }
  };
}

// A labelled type-ahead combobox (APG combobox + listbox) on the
// `useComboboxListbox` controller; skin: `.ui-text-input` + `.ui-combobox-*`
// (ui.css).
export type ComboboxFieldProps<T> = {
  label: ReactNode;
  className?: string;
  options: readonly T[];
  getId: (option: T) => string;
  getLabel: (option: T) => string;
  renderOption?: (option: T) => ReactNode;
  filter: (option: T, query: string) => boolean;
  /** Shown while the field is not being edited — e.g. the currently selected
   * option's label (owner storyboard frame 2's closed-state box). */
  displayValue: string;
  onSelect: (option: T) => void;
  escapePolicy: (state: ComboboxEscapeState) => ComboboxEscapeAction;
  /** Called for an Escape the controller left unconsumed ("bubble") — the
   * host decides what it means (the field is not a DOM descendant of the
   * host's own Escape scope). */
  onEscapeBubble?: () => void;
  placeholder?: string;
};

function ComboboxFieldInner<T>(
  {
    label,
    className,
    options,
    getId,
    getLabel,
    renderOption,
    filter,
    displayValue,
    onSelect,
    escapePolicy,
    onEscapeBubble,
    placeholder,
  }: ComboboxFieldProps<T>,
  ref: Ref<HTMLInputElement>,
) {
  const { text } = useLocale();
  const [focused, setFocused] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const controller = useComboboxListbox({ options, getId, filter, onSelect, escapePolicy });
  // `label htmlFor`, never a wrapping label: a combobox embedded in its own
  // label folds the active option into its accessible name (accname 2E), so
  // the name would change every time the list opens.
  const inputId = useId();

  // Blur after a selection: focus lands on <body>, which the "none" focus
  // intent fallback (`focusScreenHeadingIfBody`) routes to the screen heading.
  function selectAndBlur(option: T) {
    controller.select(option);
    inputRef.current?.blur();
  }

  return (
    <div className={["ui-text-field ui-combobox", className].filter(Boolean).join(" ")}>
      <label htmlFor={inputId}>{label}</label>
      <div className="ui-combobox-anchor">
        <input
          {...controller.inputProps}
          id={inputId}
          ref={mergeRefs(inputRef, ref)}
          className="ui-text-input"
          placeholder={placeholder}
          value={focused ? controller.query : displayValue}
          onChange={(event) => controller.setQuery(event.target.value)}
          onKeyDown={(event) => {
            controller.inputProps.onKeyDown(event);
            if (event.key === "Enter") inputRef.current?.blur();
            if (event.key === "Escape" && !event.defaultPrevented) onEscapeBubble?.();
          }}
          onFocus={() => setFocused(true)}
          // Opens on click, never on focus: a programmatic focus (Shift+J/K)
          // must not pop the list, and an open list's activedescendant folds
          // into the field's accessible name. ArrowDown opens it by keyboard.
          onClick={() => controller.open()}
          onBlur={() => {
            setFocused(false);
            controller.reset();
          }}
        />
        {controller.isOpen ? (
          // Keep the input focused through a mouse selection (mousedown would
          // blur it and unmount the list before click fires).
          <ul {...controller.listboxProps} className="ui-combobox-listbox" onMouseDown={(event) => event.preventDefault()}>
            {controller.filtered.length === 0 ? (
              <li className="ui-combobox-empty">{text("No matches")}</li>
            ) : (
              controller.filtered.map((option) => (
                <li
                  key={getId(option)}
                  {...controller.optionProps(option)}
                  onClick={() => selectAndBlur(option)}
                  className="ui-combobox-option"
                >
                  {renderOption ? renderOption(option) : getLabel(option)}
                </li>
              ))
            )}
          </ul>
        ) : null}
      </div>
    </div>
  );
}

export const ComboboxField = forwardRef(ComboboxFieldInner) as <T>(
  props: ComboboxFieldProps<T> & { ref?: Ref<HTMLInputElement> },
) => ReactElement;
