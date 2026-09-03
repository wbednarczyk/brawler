import type { InputHTMLAttributes, KeyboardEvent, Ref } from "react";
import { Search } from "lucide-react";
import { ClearButton } from "./ClearButton";

type SearchFieldInputProps = Omit<
  InputHTMLAttributes<HTMLInputElement>,
  "aria-label" | "className" | "onChange" | "placeholder" | "type" | "value"
> &
  Record<`data-${string}`, string | number | boolean | undefined> & {
    // Additive (F3c S2, #197): lets a caller (the command palette) obtain an
    // imperative handle for Modal's initialFocusRef — plain InputHTMLAttributes
    // has no `ref` field.
    ref?: Ref<HTMLInputElement>;
  };

export type SearchFieldProps = {
  ariaLabel: string;
  as?: "label" | "span";
  className?: string;
  clearLabel?: string;
  iconSize?: number;
  inputProps?: SearchFieldInputProps;
  onChange: (value: string) => void;
  onClear?: () => void;
  placeholder: string;
  type?: "search" | "text";
  value: string;
};

export function SearchField({
  ariaLabel,
  as = "label",
  className,
  clearLabel,
  iconSize = 15,
  inputProps,
  onChange,
  onClear,
  placeholder,
  type = "search",
  value,
}: SearchFieldProps) {
  const Component = as;

  // Escape contract (F3c S1, plan § Design 2): a non-empty query is cleared
  // and the event consumed (so it doesn't ALSO reach the Spółka tool frame's
  // Escape-to-close handler); an empty query lets Escape bubble untouched —
  // e.g. to close the hosted tool. `inputProps.onKeyDown` (CommandPalette's
  // arrow-key/Enter nav) runs first and is always respected.
  function handleKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    inputProps?.onKeyDown?.(event);
    if (event.defaultPrevented) return;
    if (event.key !== "Escape") return;
    if (value.trim().length === 0) return;
    event.preventDefault();
    event.stopPropagation();
    if (onClear) {
      onClear();
    } else {
      onChange("");
    }
  }

  return (
    <Component className={className}>
      <Search aria-hidden="true" size={iconSize} />
      <input
        {...inputProps}
        aria-label={ariaLabel}
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={handleKeyDown}
        placeholder={placeholder}
        type={type}
        value={value}
      />
      {onClear && clearLabel && value.trim().length > 0 ? (
        <ClearButton label={clearLabel} onClick={onClear} />
      ) : null}
    </Component>
  );
}
