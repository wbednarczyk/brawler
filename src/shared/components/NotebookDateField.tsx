import { DateField } from "../../ui";
import type { NotebookDateLikeFieldProps } from "../types/notebook";

// Domain wrapper: the `.date-picker-field` label layout every date-bearing form
// (decision journal, notebook, events, transcripts) shares, delegating the input
// itself to the `DateField` primitive so the control carries the design-system
// box/focus styling instead of raw native chrome (v0.52 dogfooding fix).
export function NotebookDateField({ label, ariaLabel, value, onChange }: NotebookDateLikeFieldProps) {
  return (
    <div className="date-picker-field">
      <span>{label}</span>
      <DateField
        aria-label={ariaLabel}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
    </div>
  );
}
