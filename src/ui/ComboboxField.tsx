import { forwardRef, useRef, useState, type ReactElement, type ReactNode, type Ref } from "react";

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

// A labelled type-ahead combobox (dogfooding wave 2026-09, #3): APG
// combobox + listbox, driven by the shared `useComboboxListbox` controller
// (the SAME controller the ⌘K palette uses). Structure only, reusing the
// existing generic `.ui-text-field`/`.ui-text-input` shell (ui.css) for the
// input — the popup listbox's own floating position/skin currently lives in
// `spolka.css` (its only consumer today, the Spółka company picker);
// GlobalSearch's migration onto this primitive (tracked follow-up) is the
// natural point to promote that CSS into the shared sheet.
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
  /** Called when the controller's `escapePolicy` resolves "bubble" for an
   * Escape keydown — i.e. the controller touched neither the event nor its
   * own state, and the HOST decides what happens next (the company picker's
   * "closed + empty" state, plan § S2 item 2: returns to the workshop tool
   * frame's Overview). Detected generically off `!event.defaultPrevented`
   * after the controller's own handler ran, so this needs no coupling to
   * `escapePolicy`'s specific action names. */
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

  // Storyboard frame 1 feedback: "po Enter — focus lands on the tool
  // heading/Overview" — blurring here (not left on stale typed text) hands
  // focus to <body>, which the existing "none" focus-intent fallback
  // (`focusScreenHeadingIfBody`) already picks up for a plain company
  // switch (`useSpolkaNavigate`) — no new focus plumbing needed.
  function selectAndBlur(option: T) {
    controller.select(option);
    inputRef.current?.blur();
  }

  return (
    <label className={["ui-text-field ui-combobox", className].filter(Boolean).join(" ")}>
      {label}
      <div className="ui-combobox-anchor">
        <input
          {...controller.inputProps}
          ref={mergeRefs(inputRef, ref)}
          className="ui-text-input"
          placeholder={placeholder}
          value={focused ? controller.query : displayValue}
          onChange={(event) => controller.setQuery(event.target.value)}
          onKeyDown={(event) => {
            // The hook's own handler already runs the Enter → select() path
            // (it owns activeOption); this only adds the post-selection blur
            // (see `selectAndBlur` above) without re-selecting.
            controller.inputProps.onKeyDown(event);
            if (event.key === "Enter") inputRef.current?.blur();
            if (event.key === "Escape" && !event.defaultPrevented) onEscapeBubble?.();
          }}
          onFocus={() => setFocused(true)}
          // Opens on CLICK (storyboard frame 2: "click the field → the list
          // [appears]"), not on every focus: a `.focus()` that lands here
          // programmatically (Shift+J/K's "company" focus intent) must NOT
          // pop the list open — besides being noisy for a keyboard user who
          // hasn't asked for it, an open list with an `aria-activedescendant`
          // changes this field's OWN accessible name (accname step 2H folds
          // the active option's text into it), which breaks "focused but
          // otherwise untouched" as a stable state. A keyboard user can still
          // reach the list with ArrowDown (the controller's own handler).
          onClick={() => controller.open()}
          onBlur={() => {
            setFocused(false);
            controller.reset();
          }}
        />
        {controller.isOpen ? (
          // Keeps the input focused through a mouse selection (the standard
          // combobox pattern) — without this, the mousedown's default focus
          // shift blurs the input before the click's onClick ever fires,
          // which would unmount this list before the selection registers.
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
    </label>
  );
}

export const ComboboxField = forwardRef(ComboboxFieldInner) as <T>(
  props: ComboboxFieldProps<T> & { ref?: Ref<HTMLInputElement> },
) => ReactElement;
