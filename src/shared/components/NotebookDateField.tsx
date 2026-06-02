import type { NotebookDateLikeFieldProps } from "../types/notebook";

export function NotebookDateField({ label, ariaLabel, value, onChange }: NotebookDateLikeFieldProps) {
  return (
    <div className="date-picker-field">
      <span>{label}</span>
      <input
        aria-label={ariaLabel}
        type="date"
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
    </div>
  );
}
