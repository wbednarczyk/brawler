import type { InputHTMLAttributes, Ref } from "react";
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
  return (
    <Component className={className}>
      <Search aria-hidden="true" size={iconSize} />
      <input
        {...inputProps}
        aria-label={ariaLabel}
        onChange={(event) => onChange(event.target.value)}
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
